<script lang="ts">
	// const type parameter in a Svelte <script lang="ts"> — see const_type_param/input.ts
	// for the pure-TS sibling. Function declarations take no trailing comma; only the
	// arrow's single const type param diverges (tsv bare, prettier `<const T,>` — see README).
	function literal<const T>(value: T): T {
		return value;
	}

	function tuple<const T extends readonly unknown[]>(items: T): T {
		return items;
	}

	const a = literal('');
	const b = tuple([1, 2, 3]);

	// Arrow with a single const type param — tsv bare, prettier forces `<const T,>`.
	const arrow = <const T,>(x: T) => x;
</script>
