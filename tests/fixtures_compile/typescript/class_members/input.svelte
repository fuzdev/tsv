<script lang="ts">
	class Base {
		tag = 'b';
	}

	class C extends Base {
		declare gone: number;
		readonly kept: number = 1;
		private hidden = 2;
		override tag = 'c';

		method(label?: string): void {}
	}

	let value = new C().kept;
</script>

<p>{value}</p>
