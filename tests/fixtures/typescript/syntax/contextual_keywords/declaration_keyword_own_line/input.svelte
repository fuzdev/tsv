<script lang="ts">
	// `type` and `declare` are contextual keywords: with the following token on a
	// new line, ASI demotes the keyword to a plain identifier — it does not start a
	// declaration, so the statement splits in two. The input_invalid_* variants show
	// the same line break makes interface/namespace/module headers unparseable.
	type; // c
	T = string;

	declare; // c
	const x = 1;
</script>
