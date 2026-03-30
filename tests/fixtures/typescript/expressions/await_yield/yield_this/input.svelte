<script>
	function* gen() {
		yield this;
		yield this.x;
		yield* this.items;
	}

	class A {
		*gen() {
			yield this;
			yield this.x;
			yield* this.items;
		}
	}
</script>
