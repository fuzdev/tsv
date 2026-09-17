<script>
	const q = { a: 2, b: 3 };
	// a pattern default
	const { a: a = 1, b: b } = q;
	const p = { a: a /* after the value */, b: b };
	class C {
		m() {
			// nested in a method
			return {
				a: a,
				get g() {
					return 1;
				}
			};
		}
	}
</script>

<p>{p.b}{new C().m().a}</p>
