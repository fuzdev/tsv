<script lang="ts">
// own-line comment after `<`, plus a trailing comment on the type
const a=<
// c
string // d
>x;
// line comment trailing `<`, plus a trailing comment on the type
const b=< // c
string // d
>y;
// line comment trailing `>`, before the expression
const c=<string>// c
z;
</script>
