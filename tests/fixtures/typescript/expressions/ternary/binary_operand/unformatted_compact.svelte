<script>
// Ternary as RIGHT operand of logical
const a1=a&&(b?c:d);const a2=a||(b?c:d);const a3=a??(b?c:d);

// Ternary as LEFT operand of logical
const b1=(a?b:c)&&d;const b2=(a?b:c)||d;

// Ternary in comparison operators
const c1=(a?b:c)===d;const c2=x===(a?b:c);

// Ternary in arithmetic
const d1=(a?b:c)+d;const d2=x+(a?b:c);

// Assignment as operand of logical
const e1=a&&(x=y);const e2=a||(x=y);

// Complex: ternary with logical condition
const f1=a&&(b||c?d:e);
</script>
