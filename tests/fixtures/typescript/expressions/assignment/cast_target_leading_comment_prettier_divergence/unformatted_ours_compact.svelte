<script lang="ts">
// A glued block leads the operand inline; the shell stays flat.
(/* c1 */x as T)=1;

// Every block in a leading run survives, not only the operand-glued one.
(/* c2 *//* c3 */y as T)=2;

// The same gap inside a destructuring default's target.
({a:(/* c4 */b as T)=3}=obj);

// An own-line block keeps its own line and expands the shell.
(
/* c5 */
z as T)=4;
</script>
