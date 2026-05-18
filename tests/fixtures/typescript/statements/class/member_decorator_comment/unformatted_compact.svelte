<script lang="ts">
class A{
// Block comment between decorator and property
@fn
/* c1 */
a=1;

// Inline block comment after decorator
@fn/* c2 */b=1;

// Line comment after decorator
@fn // c3
c=1;

// Block comment between decorator and method
@fn
/* c4 */
d(){}

// Inline block comment after decorator before method
@fn/* c5 */e(){}

// Line comment after decorator before method
@fn // c6
f(){}

// Comment between decorators
@fn1
/* c7 */
@fn2
g=1;

// Inline comment between decorators
@fn1
/* c8 */
@fn2
h=1;

// Comment after call expression decorator
@fn(x)
/* c9 */
i=1;

// Inline comment after call expression decorator
@fn(x)/* c10 */j=1;
}
</script>
