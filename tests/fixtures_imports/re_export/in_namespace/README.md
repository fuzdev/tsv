A re-export inside a namespace body is no record: tsc rejects both forms there (TS1194), though both parsers accept them. Nor is either module syntax, which reads the top level alone.
