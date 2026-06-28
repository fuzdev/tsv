<script>
	// inline block comment — parens preserved for comment
	const a=()=>(x/* c */);

	// curried arrow — outer parens preserved for comment
	const b=(a)=>((b)=>z/* c */);

	// multiple levels — each level preserves parens
	const c=(a)=>((b)=>((c)=>w/* c1 */)/* c2 */);

	// line comment — forces multiline parens
	const d=()=>(x// c
	);

	// own-line block comment — forces multiline parens
	const e=()=>(x
/* c */
	);

	// stacked line comments — each kept on its own line (no swallow)
	const g=()=>(x// c1
// c2
	);

	// leading + trailing — leading hugs `=>`, trailing keeps parens
	const h=()=>/* lead */(x/* trail */);

	// same-line comment group after a body break — block + line stay together
	const i=()=>(x
/* i1 */ // i2
	);

	// object-literal body — parens are required, comment kept inside (not after `)`)
	const j=()=>({k:1}/* c */);

	// inside call arg — parens preserved
	f(()=>(x/* c */));
</script>
