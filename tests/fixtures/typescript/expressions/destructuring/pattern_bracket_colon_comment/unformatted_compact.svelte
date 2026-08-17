<script lang="ts">
	// A block comment between a destructuring pattern's closing bracket and its
	// `: Type` annotation belongs to the binding, not to the last element.
	const {a}/* c1 */: T = x;
	const [b]/* c2 */: U = y;

	// An empty pattern keeps it outside the brackets too.
	const {}/* c3 */: V = z;
	const []/* c4 */: W = w;

	// The same gap after an optional `?` marker, where a binding pattern may take one.
	type F1 = ({c}?/* c5 */: A) => void;
	type F2 = ([]?/* c6 */: B) => void;

	// A parameter's own gap, on the value side.
	function fn({d}/* c7 */: C) {}
</script>
