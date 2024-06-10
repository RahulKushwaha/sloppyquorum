+++
title = "About"
description = ""
date = "2024-05-26"
aliases = ["about-us", "contact"]
author = "Rahul Kushwaha"
+++

# Rahul Kushwaha

## Professional

I am a Staff Software Engineer at Reddit working in the Caching &
Storage Infra Org.
My current work involves providing Redis as an infrastructure
using Kubernetes.

Previously, I worked with Meta/Facebook on their low-dependency
metadata storage system called [DelosTable](https://engineering.fb.com/2019/06/06/data-center-engineering/delos/). This
system provides a table-like(Relational) abstraction to our
customers requiring a highly available & linearizable store.

Before that I was working at OfferUp handling the Identity & User
Domain.

For more details of my work: [Resume](./rahul_kushwaha_resume.pdf)

## Personal
### MyDb
I am also writing my own Replicated Database as a hobby. It is still
in its infancy but has a Multi-Paxos implementation for Log with
dynamic reconfiguration. It provides table abstraction using RocksDb, and
uses Apache Arrow for query execution & data handling.
https://github.com/RahulKushwaha/replicated_counter.

### Innodb Experiments
I have been experimenting with Innodb to understand its internals, mostly around Concurrency-Control and Logging. I 
have made the following contributions to its fork called [Embedded Innodb](https://github.com/sunbains/embedded-innodb)

[My Contributions](https://github.com/sunbains/embedded-innodb/pulls?q=is%3Apr+is%3Aclosed+author%3ARahulKushwaha)

### DistSys Reading Group
I am an active member of the DistSys Reading Group. We read and discuss one distributed systems paper every week.
Come & join us: [Link](https://charap.co/reading-group)

#### My Presentations:
1. [Polaris: Enabling Transaction Priority in Optimistic Concurrency Control](https://pages.cs.wisc.edu/~chenhaoy/publication/polaris/)  
[Slides](./polaris_slides.pdf)  
[![](https://markdown-videos-api.jorgenkh.no/youtube/E9BJF9Tu6n8)](https://www.youtube.com/embed/E9BJF9Tu6n8?si=AqKcty9Pfm7iVMFr)
2. [Morty: Scaling Concurrency Control with Re-Execution](https://www.cs.cornell.edu/lorenzo/papers/Burke23Morty.pdf)  
[Slides](./morty_slides.pdf)  
[![](https://markdown-videos-api.jorgenkh.no/youtube/49QJSkrMKNc)](https://youtu.be/49QJSkrMKNc?si=yOe6tVt_tLuZ9x1D)
3. *[Coming Soon]* [Understanding the Performance Implications of the Design Principles in Storage-Disaggregated Databases](https://www.cs.purdue.edu/homes/csjgwang/pubs/SIGMOD24_OpenAurora.pdf)

### Database Book Club
I actively participate, and help run a book club where we discuss all-things-databases.
**Currently reading**: *Transaction Processing: Concepts and Techniques, by Jim Gray & Andreas Reuter*

Come & join us. [Discord]( https://databass.dev/discord)

You can find me here:
1. [Twitter/X](https://twitter.com/sloppyquorum)
2. [Github](https://github.com/RahulKushwaha/)
3. [LinkedIn](https://www.linkedin.com/in/rahul-kushwaha-589a0741/)