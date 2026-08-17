<script>
// trailing block comment on the left operand of a parenthesized binary member base
const a1=((x&&y)/* c */||z).toString();
const a2=((x+y)/* c *// z).toString();

// leading block comment on the right operand
const b1=((x&&y)||/* c */z).toString();
const b2=((x+y) / /* c */z).toString();

// a line comment before the operator breaks the base onto its own lines
const c=((x&&y)|| // c
z).toString();

// the same comment on a base too long to fit
const d=((aaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbb)/* c */||zzzzzzzzzzzzzz).toString();
</script>
