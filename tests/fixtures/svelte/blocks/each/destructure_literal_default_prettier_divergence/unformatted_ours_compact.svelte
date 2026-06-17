<!-- string defaults normalize to single quotes; escape-minimizing keeps double for a'b -->
{#each items as {a="x",b='a"b',c="a'b"}}<div>x</div>{/each}

<!-- numeric defaults normalize to canonical form (lowercase hex/exponent, leading/trailing zeros, bigint) -->
{#each items as {a=0xFF,b=1.50,c=.5,d=1E10,e=0xFFn}}<div>x</div>{/each}

<!-- literals nested in object/array default values normalize recursively -->
{#each items as {a={x:"y",z:1.50},b=[0xFF,"w"]}}<div>x</div>{/each}

<!-- array-pattern defaults normalize too (no bracket spacing in either formatter) -->
{#each items as [a="x",b=0xFF]}<div>x</div>{/each}

<!-- booleans, null, and regex are already canonical — unchanged through the TS printer -->
{#each items as {a=true,b=null,c=/Ab/gi}}<div>x</div>{/each}
