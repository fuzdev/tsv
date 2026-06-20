<script lang="ts">
// generic cast type with a nested `>` — the cast's close `>` is found past the type's own `>`
const a=<Map<string, number>// c
>x;
// object operand after `>` goes on the continuation line (not the cast-hug layout)
const b=<T>// c
{x: 1};
// block + line comment together trailing `<`, both kept on the `<` line
const c=</* b */// c
string>x;
</script>
