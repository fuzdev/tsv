<script lang="ts">
// A `//` the author glued to the `(` of a redundant shell around the true branch stays
// in the `?` gap, trailing the operator — where the paren-free authoring puts it
type A=T extends U?(// c
V):W;

// ...and around the false branch, in the `:` gap
type B=T extends U?V:(// c
W);

// A run of two stays together, one comment per line
type C=T extends U?(// c1
// c2
V):W;

// A comment already trailing the extends-type or the true branch keeps its line; the
// shell's comment stays in its gap below it
type D=T extends U// c2
?(// c1
V):W;
type E=T extends U?V// c2
:(// c1
W);

// A shell around a nested conditional strips the same way; the nested conditional
// breaks below the comment
type F=T extends U?(// c
V extends W?X:Y):Z;
type G=T extends U?Z:(// c
V extends W?X:Y);

// A shell whose trailing gap holds a `//` too: in the true position the shell strips (the
// outer `:` flushes that comment), so the leading one takes the gap the same way
type H=T extends U?(// c1
V extends W?X:Y // c2
):Z;
</script>
