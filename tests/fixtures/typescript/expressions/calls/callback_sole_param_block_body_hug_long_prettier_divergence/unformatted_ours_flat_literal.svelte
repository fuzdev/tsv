<script lang="ts">
	// At 100 the broken-out argument line holds the flat object type, so the arguments break out
	// and the object type stays flat
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// At 101 the flat object type fits on no line, so its break counts as authored: the arrow
	// keeps the hug and the object type expands inside it
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// An empty body ends the broken-out argument line at its `}`: at 100 that line holds the flat
	// object type
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {});

	// At 101 it does not, so the arrow keeps the hug
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {});

	// The first argument's broken-out line ends at its comma: at 100 it holds the flat object type
	reduce((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {}, '');

	// At 101 it does not, so the arrow keeps the hug
	reduce((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {}, '');

	// a lone arrow
	fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// new
	new Xy((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// member chain
	obj.a.fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// a return type
	fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): void => {
		g(a);
	});

	// a two-call member chain
	obj.a().fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// An expanded member chain puts the broken-out argument line a level deeper: at 100 it holds
	// the flat object type
	obj.a().fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// At 101 it does not: the chain stays flat and the arrow keeps the hug
	obj.a().fn((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// A two-call member chain's first argument
	obj.a().reduce((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		return 1;
	}, '');

	// the first argument
	reduce((a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		return 1;
	}, '');

	// the object type annotating a sole destructured parameter
	fn(lead, ({ x }: { x: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(x);
	});

	// a function callback with a preceding argument
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) {
		g(a);
	});

	// A `function` callback whose parameter groups on its own ends the broken-out argument line at
	// its object return type's `{`: at 100 that line holds the flat object type, and the return
	// type breaks instead
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// At 101 it does not, so the callback keeps the hug
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// In an expanded member chain that line sits a level deeper: at 100 it holds the flat object
	// type
	const v = obj.a().fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// At 101 it does not: the chain stays flat and the callback keeps the hug
	const w = obj.a().fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// control: under `new` the hug keeps the parameters breakable, so the object type breaks
	// inside it in both formatters
	new Xy(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// An empty nested object type has nothing to break, so the arrow keeps the hug
	fn(lead, (a: { b: {}; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	});

	// control: an expression body breaks every argument out, the object type broken by width
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => g(a));

	// control: a nested object type breaks for width inside the broken-out argument, and that
	// break refuses the hug in both formatters, so the arguments stay broken out
	fn(lead, (a: { b: { c: D; e: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }; d: F }) => {
		g(a);
	});

	// control: a nested mapped type, the same
	fn(lead, (a: { b: { [K in X]: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }; d: F }) => {
		g(a);
	});

	// control: an object type annotating a nested function type's parameter, the same
	fn(lead, (a: { b: (x: { c: D; e: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => void; d: F }) => {
		g(a);
	});

	// control: a lone function callback hugs in both formatters
	fn(function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) {
		g(a);
	});
</script>

<!-- a template expression -->
<p>
	{fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }) => {
		g(a);
	})}
</p>
