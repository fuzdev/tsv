<script lang="ts">
	declare class Single implements Base {
		prop: string;
	}

	declare class Multiple implements Base, Other {
		prop: string;
	}

	declare class WithGenerics<T> implements Base<T> {
		value: T;
	}

	declare class MultipleWithGenerics<T> implements Base<T>, Other<T> {
		value: T;
	}
</script>
