<script lang="ts">
	function identity<T>(value: T): T {
		return value;
	}

	let a = identity<number>(1);
	let f = identity<string>;
</script>

<p>{a}{f}</p>
