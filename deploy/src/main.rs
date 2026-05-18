// One binary for building, link-checking, and deploying the
// Hugo site. Shells out to `hugo` and `lychee` (already present
// on dev machines and in CI) and talks to AWS via the SDK.
//
//   deploy build              # hugo --minify --gc --cleanDestinationDir
//   deploy check              # build + leak grep + lychee
//   deploy push --bucket B    # build + check + s3 upload + cf invalidate
//                --distributions D1,D2
//
// AWS credentials come from the default chain (env vars,
// ~/.aws/credentials, IMDS, etc.).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use aws_sdk_cloudfront::types::{InvalidationBatch, Paths};
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use clap::{Args, Parser, Subcommand};
use futures::stream::{self, StreamExt};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(about = "Build, link-check, and deploy the Hugo site.")]
struct Cli {
    /// Repository root. Defaults to the current directory.
    #[arg(long, global = true)]
    repo: Option<PathBuf>,

    /// Hugo output directory, relative to --repo.
    #[arg(long, global = true, default_value = "public")]
    public_dir: PathBuf,

    /// Production base URL. Used to remap site-absolute links to
    /// local files during the offline link check.
    #[arg(long, global = true, default_value = "https://sloppyquorum.com/")]
    base_url: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run `hugo --minify --gc --cleanDestinationDir`.
    Build,
    /// Build, then dev-URL leak grep and lychee.
    Check,
    /// Build + check, then upload to S3 and invalidate CloudFront.
    Push(PushArgs),
}

#[derive(Args, Debug)]
struct PushArgs {
    /// Target S3 bucket.
    #[arg(long, env = "DEPLOY_BUCKET")]
    bucket: String,

    /// CloudFront distribution IDs, comma-separated.
    #[arg(long, value_delimiter = ',', env = "DEPLOY_DISTRIBUTIONS")]
    distributions: Vec<String>,

    /// Bitwarden item name (or id) holding the AWS credentials.
    /// Expected schema: Login item where `username` is the
    /// access key id, `password` is the secret access key, and
    /// (optionally) a custom field named `AWS_SESSION_TOKEN`
    /// holds an STS session token. `BW_SESSION` must be exported
    /// (e.g. `export BW_SESSION=$(bw unlock --raw)`).
    #[arg(long, env = "DEPLOY_BITWARDEN_ITEM")]
    bitwarden_item: Option<String>,

    /// Don't delete S3 keys missing from public/.
    #[arg(long)]
    no_prune: bool,

    /// Log actions without calling AWS.
    #[arg(long)]
    dry_run: bool,

    /// Max concurrent uploads.
    #[arg(long, default_value_t = 16)]
    concurrency: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = match cli.repo.as_ref() {
        Some(p) => p
            .canonicalize()
            .with_context(|| format!("--repo {}", p.display()))?,
        None => std::env::current_dir()?,
    };
    let public = repo.join(&cli.public_dir);

    match cli.cmd {
        Cmd::Build => run_hugo(&repo)?,
        Cmd::Check => {
            run_hugo(&repo)?;
            grep_dev_url_leaks(&public)?;
            run_lychee(&repo, &public, &cli.base_url)?;
        }
        Cmd::Push(args) => {
            if args.distributions.is_empty() {
                bail!("--distributions must list at least one CloudFront ID");
            }
            run_hugo(&repo)?;
            grep_dev_url_leaks(&public)?;
            run_lychee(&repo, &public, &cli.base_url)?;
            push(&public, &args).await?;
        }
    }
    Ok(())
}

fn run_hugo(repo: &Path) -> Result<()> {
    println!("$ hugo --minify --gc --cleanDestinationDir");
    let status = Command::new("hugo")
        .args(["--minify", "--gc", "--cleanDestinationDir"])
        .current_dir(repo)
        .status()
        .context("spawn hugo (is it on PATH?)")?;
    if !status.success() {
        bail!("hugo exited with {:?}", status.code());
    }
    Ok(())
}

fn grep_dev_url_leaks(public: &Path) -> Result<()> {
    let mut leaks = Vec::new();
    for entry in WalkDir::new(public).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "html" | "xml") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(path) else {
            continue;
        };
        if body.contains("http://localhost:") || body.contains("http://127.0.0.1") {
            leaks.push(path.display().to_string());
        }
    }
    if !leaks.is_empty() {
        eprintln!("dev baseURL leaked into public/:");
        for l in &leaks {
            eprintln!("  {l}");
        }
        bail!("{} file(s) contain dev URLs", leaks.len());
    }
    println!("leak guard: clean");
    Ok(())
}

fn run_lychee(repo: &Path, public: &Path, base_url: &str) -> Result<()> {
    let public_abs = public.display().to_string();
    let remap = format!("{base_url} file://{public_abs}/");
    println!("$ lychee --offline (remap={remap})");
    let status = Command::new("lychee")
        .arg("--config").arg("lychee.toml")
        .arg("--root-dir").arg(&public_abs)
        .arg("--remap").arg(&remap)
        .arg("--offline")
        .arg("./public/**/*.html")
        .arg("./public/**/*.xml")
        .current_dir(repo)
        .status()
        .context("spawn lychee (install with `cargo install lychee`)")?;
    if !status.success() {
        bail!("lychee reported broken links");
    }
    Ok(())
}

async fn push(public: &Path, args: &PushArgs) -> Result<()> {
    let local: Vec<(String, PathBuf)> = WalkDir::new(public)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let abs = e.into_path();
            let rel = abs
                .strip_prefix(public)
                .expect("walkdir entry under public_dir")
                .to_path_buf();
            let key = rel.to_string_lossy().replace('\\', "/");
            (key, abs)
        })
        .collect();

    println!(
        "{} {} files -> s3://{}/",
        if args.dry_run { "DRY-RUN upload" } else { "Uploading" },
        local.len(),
        args.bucket,
    );

    if args.dry_run {
        for (k, _) in &local {
            println!("  + {k}");
        }
        for d in &args.distributions {
            println!("  invalidate {d} /*");
        }
        return Ok(());
    }

    let aws_cfg = match args.bitwarden_item.as_deref() {
        Some(item) => {
            let creds = fetch_bw_credentials(item)?;
            aws_config::from_env().credentials_provider(creds).load().await
        }
        None => aws_config::from_env().load().await,
    };
    let s3 = aws_sdk_s3::Client::new(&aws_cfg);
    let cf = aws_sdk_cloudfront::Client::new(&aws_cfg);

    upload_all(&s3, &args.bucket, &local, args.concurrency).await?;
    if !args.no_prune {
        prune(&s3, &args.bucket, &local).await?;
    }
    invalidate(&cf, &args.distributions).await?;
    Ok(())
}

async fn upload_all(
    s3: &aws_sdk_s3::Client,
    bucket: &str,
    local: &[(String, PathBuf)],
    concurrency: usize,
) -> Result<()> {
    let results: Vec<Result<String>> = stream::iter(local.iter().cloned())
        .map(|(key, path)| {
            let s3 = s3.clone();
            let bucket = bucket.to_string();
            async move {
                let body = ByteStream::from_path(&path)
                    .await
                    .with_context(|| format!("read {}", path.display()))?;
                let content_type = mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .essence_str()
                    .to_string();
                s3.put_object()
                    .bucket(&bucket)
                    .key(&key)
                    .body(body)
                    .content_type(content_type)
                    .send()
                    .await
                    .with_context(|| format!("put_object {key}"))?;
                Ok(key)
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let mut failures = 0;
    for r in results {
        match r {
            Ok(key) => println!("  put: {key}"),
            Err(e) => {
                eprintln!("  FAIL: {e:#}");
                failures += 1;
            }
        }
    }
    if failures > 0 {
        bail!("{failures} uploads failed");
    }
    Ok(())
}

async fn prune(
    s3: &aws_sdk_s3::Client,
    bucket: &str,
    local: &[(String, PathBuf)],
) -> Result<()> {
    let local_keys: HashSet<&str> = local.iter().map(|(k, _)| k.as_str()).collect();

    let mut stale = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut req = s3.list_objects_v2().bucket(bucket);
        if let Some(t) = token.as_ref() {
            req = req.continuation_token(t);
        }
        let resp = req.send().await.context("list_objects_v2")?;
        for obj in resp.contents() {
            if let Some(k) = obj.key() {
                if !local_keys.contains(k) {
                    stale.push(k.to_string());
                }
            }
        }
        match resp.next_continuation_token() {
            Some(t) => token = Some(t.to_string()),
            None => break,
        }
    }

    println!("Pruning {} stale objects", stale.len());
    for key in &stale {
        s3.delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("delete_object {key}"))?;
        println!("  del: {key}");
    }
    Ok(())
}

async fn invalidate(
    cf: &aws_sdk_cloudfront::Client,
    distributions: &[String],
) -> Result<()> {
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_millis();

    for dist in distributions {
        let paths = Paths::builder()
            .quantity(1)
            .items("/*")
            .build()
            .context("build invalidation paths")?;
        let batch = InvalidationBatch::builder()
            .paths(paths)
            .caller_reference(format!("deploy-{dist}-{now_ms}"))
            .build()
            .context("build invalidation batch")?;
        let resp = cf
            .create_invalidation()
            .distribution_id(dist)
            .invalidation_batch(batch)
            .send()
            .await
            .with_context(|| format!("create_invalidation {dist}"))?;
        let inv = resp
            .invalidation()
            .context("invalidation response missing body")?;
        println!(
            "Invalidation {} on {}: {}",
            inv.id(),
            dist,
            inv.status(),
        );
    }
    Ok(())
}

// Pull AWS credentials from a Bitwarden vault item. Shells out to
// the Bitwarden CLI (`bw`), which reads its session from BW_SESSION
// in the environment, parses the item's JSON, and builds a static
// credential provider for the AWS SDK.
fn fetch_bw_credentials(item: &str) -> Result<Credentials> {
    if std::env::var("BW_SESSION").is_err() {
        bail!(
            "BW_SESSION not set. Run `export BW_SESSION=$(bw unlock --raw)` first."
        );
    }
    let out = Command::new("bw")
        .args(["get", "item", item, "--raw"])
        .output()
        .context("spawn bw (install the Bitwarden CLI and ensure it is on PATH)")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("bw get item {item} failed: {}", stderr.trim());
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("parse bw item JSON")?;
    let login = v.get("login").context("vault item has no `login` section")?;
    let access_key = login
        .get("username")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .context("login.username is empty (put the AWS access key id there)")?;
    let secret = login
        .get("password")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .context("login.password is empty (put the AWS secret access key there)")?;
    let session = v
        .get("fields")
        .and_then(|f| f.as_array())
        .and_then(|fields| {
            fields
                .iter()
                .find(|f| f.get("name").and_then(|n| n.as_str()) == Some("AWS_SESSION_TOKEN"))
                .and_then(|f| f.get("value"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        });
    Ok(Credentials::new(
        access_key.to_string(),
        secret.to_string(),
        session,
        None,
        "bitwarden",
    ))
}
