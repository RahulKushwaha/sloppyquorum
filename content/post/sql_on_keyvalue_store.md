+++
author = "Rahul Kushwaha"
title = "Map relational table on a key value store"
date = "2024-02-05"
description = "How to map a Relational Table on a Key-Value Store? "
tags = [
    "database"
]
draft = true
summary = "Key-value stores provide a generic API to store and retrieve values using the key. In this post, we will see how relational tables can be mapped to key-value stores. "
+++

To implement table abstraction on top of the KeyValue store, we need to map table rows to a set of key values. We can use the following example as an illustration.

Table:

| Id | FirstName | LastName | Address | SSN  |
|----|-----------|----------|---------|------|
| 1  | Mary      | Jane     | Heaven  | 9088 |
| 2  | John      | Doe      | Hell    | 9678 |

## Primary Key

Primary Key: Id

| Key              | Value  |
|------------------|--------|
| 1/Mary/FirstName | Mary   |
| 1/Mary/LastName  | Jane   |
| 1/Mary/Address   | Heaven |
| 1/Mary/SSN       | 9088   |
| 2/John/FirstName | John   |
| 2/John/LastName  | Doe    |
| 2/John/Address   | Hell   |
| 2/John/SSN       | 9678   |

As we can see in the above diagram, we are converting a single row to a bunch of key value pairs. We can think of table rows as a N x M matrix where N is the number of rows and M is the number of columns, and keypairs as a K x 2 matrix where K is the total number of key value pairs.

Format

**Key**: db_id/tbl_id/primary_col_id/primary_col_value/col_id \
**Value**: column value

Using the above formatting scheme we can convert each column value of a row into a unique key value pair. Prepending db_id and tbl_id to each key is to make sure that we can prevent collisions between different tables and databases.

## Composite Primary Key

The above example showcased a primary key that is composed of only one column. When a row is uniquely identified by a bunch of columns, it is called a Composite Primary Key.

Table:

| FirstName | LastName | Address | SSN  |
|-----------|----------|---------|------|
| Mary      | Jane     | Heaven  | 9088 |
| John      | Doe      | Hell    | 9678 |

**Primary Key**: (FirstName, LastName)

Therefore, to create a unique key we have to utilize all the primary
column values.

Format

**Key**: db_id/tbl_id/primary_col1_id/primary_col1_value
/primary_col2_id
/primary_col2_value/primary_col3_id/primary_col3_value/col_id \
**Value**: column value

## Secondary Index

Secondary indexes provide another way to query the table using another
set of columns in addition to the primary key index.

Format

**Key**: db_id/tbl_id/secondary_index_col1_id/secondary_index_col1_value/secondary_index_col2_id/secondary_index_col2_value/secondary_index_col3_id/secondary_index_col3_value/primary_col1_id/primary_col1_value/primary_col2_id/primary_col2_value/primary_col3_id/primary_col3_value/ \
**Value**: NULL

We would like to make sure that all the rows corresponding to a
particular index are clubbed together. To ensure this grouping we can
introduce index_id that uniquely identifies an index and use it in the
key prefix.

Primary Key Index

Format

**Key**:
db_id/tbl_id/index_id/primary_col1_id/primary_col1_value/primary_col2_id/primary_col2_value/primary_col3_id/primary_col3_value/col_id
Value: column value

Secondary Key Index

Format

**Key**:
db_id/tbl_id/index_id/secondary_index_col1_id/secondary_index_col1_value/secondary_index_col2_id/secondary_index_col2_value/secondary_index_col3_id/secondary_index_col3_value/primary_col1_id/primary_col1_value/primary_col2_id/primary_col2_value/primary_col3_id/primary_col3_value/
**Value**: NULL

All the keys belonging to a particular index will be grouped together
as they will have the same prefix, db_id/tbl_id/index_id.

Let’s apply the above logic to a sample table row.

Table:

| FirstName | LastName | Address | SSN  |
|-----------|----------|---------|------|
| Mary      | Jane     | Heaven  | 9088 |
| John      | Doe      | Hell    | 9678 |

Db Index: 1 \
Table Index: 1 \
Primary Key Index: \
Id: 0 \
Columns: (FirstName, LastName)

Secondary Key Index: \
Id: 1 \
Columns: (SSN, LastName)

| Key                                                       | Value  |
|-----------------------------------------------------------|--------|
| 1/1/0/FirstName/Mary/LastName/Jane/FirstName              | Mary   |
| 1/1/0/FirstName/Mary/LastName/Jane/LastName               | Jane   |
| 1/1/0/FirstName/Mary/LastName/Jane/Address                | Heaven |
| 1/1/0/FirstName/Mary/LastName/Jane/SSN                    | 9088   |
| 1/1/0/FirstName/John/LastName/Doe/FirstName               | John   |
| 1/1/0/FirstName/John/LastName/Doe/LastName                | Doe    |
| 1/1/0/FirstName/John/LastName/Doe/Address                 | Hell   |
| 1/1/0/FirstName/John/LastName/Doe/SSN                     | 9678   |
| 1/1/1/SSN/9088/LastName/Jane/FirstName/Mary/LastName/Jane | NULL   |
| 1/1/1/SSN/9678/LastName/Doe/FirstName/John/LastName/Doe   | NULL   |

## Query Patterns

#### Pattern #1

Fetch rows using primary keys.

#### Pattern #2

Fetch all the rows using the primary key index.
All the keys matching the prefix db_id/tbl_id/index_id.

#### Pattern #3

Fetch all the rows using the secondary key index.
All the keys matching the prefix db_id/tbl_id/index_id.
The keys fetched above will allow us to fetch the primary keys of the
corresponding rows.
Using the primary keys, Pattern #1, we can fetch rows corresponding to
the primary keys. 

