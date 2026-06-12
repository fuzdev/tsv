<!-- @debug NEVER wraps - stays inline regardless of length (prettier behavior) -->

<!-- 100 chars - stays inline -->
{@debug aaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbb,ccccccccccccccccccccc,dddddddddddddddddddddd}

<!-- 101 chars - still stays inline (no wrapping for @debug) -->
{@debug aaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbb,ccccccccccccccccccccc,ddddddddddddddddddddddd}

<!-- Very long - still stays inline -->
{@debug aaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbb,ccccccccccccccccccccc,dddddddddddddddddddddd,eeeeeeeeeeeeeeeeeeeee}
