<script>
	// A `;` inside a nested block is not one of the for-header's separators — those are the
	// two at header level. A comment trailing a clause stays with that clause, before the `;`.

	// nested block in the init clause
	for (
		let f = () => {
			a();
			b();
		} /* c */;
		g();
		i++
	) {}

	// nested block in the test clause
	for (
		i = 0;
		check(() => {
			a();
			b();
		}) /* c */;
		i++
	) {}

	// nested block in the update clause
	for (
		i = 0;
		g();
		update(() => {
			a();
			b();
		}) /* c */
	) {}

	// a function expression body behaves the same
	for (
		let f = function () {
			a();
			b();
		} /* c */;
		g();
		i++
	) {}

	// no nesting — the header separators are unambiguous
	for (let f = mk() /* c */; g(); i++) {}
</script>
