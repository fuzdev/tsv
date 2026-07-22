<script lang="ts">
	abstract class Base {
		abstract run(): void;
		value = 1;
	}

	class Impl extends Base {
		run() {}
	}

	let a = new Impl().value;
</script>

<p>{a}</p>
