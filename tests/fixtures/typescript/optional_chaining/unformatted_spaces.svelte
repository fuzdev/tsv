<script>
	// Basic optional chaining
	const a = obj ?.prop;
	const b = arr ?. [0];
	const c = fn ?. ();

	// Chained optional access
	const d = a ?. b ?. c;
	const e = obj ?. prop ?. nested ?. deep;

	// Mixed chains (optional and regular)
	const f = a ?.b . c;
	const g = a . b ?.c;
	const h = a . b ?. c . d;

	// Optional computed access
	const i = obj ?. [ key ];
	const j = arr ?. [ index ] ?. value;
	const k = config ?. [ section ] ?. [ item ];

	// Optional method calls
	const l = obj ?. method(  );
	const m = obj ?. method ?. (  );
	const n = service ?. getData ?. ();

	// Chained method calls
	const o = obj ?. first ?. second(  );
	const p = api ?. fetch(  ) ?. json ?. ();

	// Optional call with arguments
	const q = fn ?. ( 1 , 2 , 3 );
	const r = obj ?. method ?. ( a , b , c );

	// Complex expressions
	const s = items ?. [ 0 ] ?. name;
	const t = data ?. results ?. [ 0 ] ?. value;
	const u = config ?. settings ?. [ key ] ?. trim ?. ();
</script>
