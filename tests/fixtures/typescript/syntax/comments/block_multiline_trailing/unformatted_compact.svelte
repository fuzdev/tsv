<script>
// block comment followed by same-line block comment
const a=1;/*
comment
*//* after */
const b=2;

// block comment followed by multiple same-line block comments
const c=1;/*
comment
*//* after1 *//* after2 */
const d=2;

// block comment followed by same-line line comment
const e=1;/*
comment
*/// line comment
const f=2;

// non-multi-line same-line block comments (control case - works)
const g=1;/* c1 *//* c2 */
const h=2;

// multi-line block comment as own-line with trailing comment
/*
comment
*//* after */
const i=1;
</script>
