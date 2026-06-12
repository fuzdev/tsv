<script lang="ts">
// Comment between accessor keyword and name is preserved
interface I{get/* c1 */a():A;set/* c2 */b(v:A);}
</script>
