<script lang="ts">
// A comma inside a block comment is not the argument separator: both
// before-comma blocks stay before the real comma.
fn(a/* p, q */,b);
fn(a/* p, q *//* r */,b);
</script>
