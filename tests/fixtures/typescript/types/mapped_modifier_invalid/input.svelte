<script lang="ts">
	// Mapped-type `+`/`-` modifiers must be followed by `readonly` (key position)
	// or `?` (value position); each position takes a single optional sign
	type Plus = { +readonly [K in S]+?: V };
	type Minus = { -readonly [K in S]-?: V };
	type Bare = { readonly [K in S]?: V };
</script>
