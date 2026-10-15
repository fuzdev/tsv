# bom

A leading byte-order mark stays; positions count it (tsv's TypeScript coordinates count a BOM, as acorn does).

## Divergence

Node drops the BOM from its output, shifting every column of the first line by one. tsv follows ts-blank-space, which keeps it.
