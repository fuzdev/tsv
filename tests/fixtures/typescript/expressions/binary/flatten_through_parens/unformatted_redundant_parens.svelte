<script>
// Redundant parens around same-precedence logical should be removed
const a1 = a && (b && c);
const a2 = a || (b || c);
const a3 = a ?? (b ?? c);

// Arithmetic keeps parens (not associative for floats)
const b1 = a + (b + c);

// Bitwise keeps parens
const b2 = a | (b | c);
const b3 = a & (b & c);

// Parens around different-precedence inner should be kept
const c1 = a && (b || c);
const c2 = a * (b + c);

// Flatten through parens, but keep parens around ternary
const d1 = a && (b && (c ? d : e));
const d2 = a || (b || (c ? d : e));
const d3 = a ?? (b ?? (c ? d : e));

// Multiple levels
const e1 = a && (b && (c && d));
const e2 = a && (b && (c && (d ? e : f)));
</script>
