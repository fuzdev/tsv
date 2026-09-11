<script>
// A line comment authored on a property's `:` line trails the `:`; the value hangs
// one level in below it.
const a={k:// c1
b,j:1};

// A run keeps its first comment on the `:` line and the rest on their own lines, in
// order, with an author blank before the value.
const c={k:// c2
// c3

b};

// A block ahead of the line comment on the `:` line stays there with it.
const d={k:/* c4 */// c5
b};

// Computed and quoted keys take the same gap.
const e={[k]:// c6
b,'a-b':// c7
b};

// An object value and a function value hang below the comment too.
const f={k:// c8
{a:1},m:// c9
()=>1};

// A property of an object passed as an argument.
fn({k:// c10
b});
</script>
