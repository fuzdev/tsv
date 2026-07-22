<script>
	class Model {
		a = $state(1);
		label = 'hi';
		b = $state(2);
		static kind = 'model';
		total() {
			return this.a + this.b;
		}
	}
	const m = new Model();
</script>

<p>{m.label}: {m.total()}</p>
