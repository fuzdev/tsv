<script lang="ts">
	// Member call + typed arrow: 100 chars - stays inline
	const a = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((...args: Array<any>): Promise<any> => fn(undefined))
	};

	// Member call + typed arrow: 101 chars - breaks at open paren
	const b = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((...args: Array<any>): Promise<any> => fn(undefined))
	};

	// Plain call + typed arrow: 101 chars - also breaks at open paren (contrast: no member expr)
	const c = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn((...args: Array<any>): Promise<any> => fn(undefined))
	};

	// Member call + untyped arrow: stays inline (contrast: no return type)
	const d = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((...args: Array<any>) => fn(undefined))
	};
</script>
