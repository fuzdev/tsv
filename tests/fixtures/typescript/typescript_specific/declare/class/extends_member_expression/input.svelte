<script lang="ts">
	declare class Single extends ns.Base {
		prop: string;
	}

	declare class Nested extends a.b.Base {
		prop: string;
	}

	declare class WithTypeArgs extends ns.Base<T> {
		value: T;
	}
</script>
