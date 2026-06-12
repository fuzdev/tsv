<!-- Single identifier parameter -->

<!-- 100 chars - stays inline (at boundary) -->
<div use:action={veryLongIdentifierNameThatExtendsToExactlyOneHundredCharactersXXXXXXXXXXXXX}></div>

<!-- 101 chars - wraps (exceeds print_width) -->
<div
	use:action={veryLongIdentifierNameThatExtendsToExactlyOneHundredCharactersXXXXXXXXXXXXXX}
></div>

<!-- Object parameter -->

<!-- 100 chars - object stays inline -->
<div use:action={{a: 'a', b: 'b', c: 'c', d: 'd', e: 'e', f: 'f', g: 'g', h: 'hhhhhhhhhhhh'}}></div>

<!-- 101 chars - element wraps, object on its own line -->
<div
	use:action={{a: 'a', b: 'b', c: 'c', d: 'd', e: 'e', f: 'f', g: 'g', h: 'hhhhhhhhhhhhh'}}
></div>

<!-- Object properties wrap when attribute line exceeds 100 chars -->

<!-- 100 chars attribute line - object inline -->
<div
	use:action={{a: 'a', b: 'b', c: 'c', d: 'd', e: 'e', f: 'f', g: 'g', h: 'hhhhhhhhhhhhhhhhhhhhhh'}}
></div>

<!-- 101 chars attribute line - object properties wrap -->
<div
	use:action={{
		a: 'a',
		b: 'b',
		c: 'c',
		d: 'd',
		e: 'e',
		f: 'f',
		g: 'g',
		h: 'hhhhhhhhhhhhhhhhhhhhhhh',
	}}
></div>

<!-- Multiple use directives -->

<!-- 100 chars - stays inline (at boundary) -->
<div use:a={p} use:b={p} use:c={p} use:d={p} use:e={p} use:f={p} use:g={p} use:h={pppppppppp}></div>

<!-- 101 chars - wraps to one directive per line -->
<div
	use:a={p}
	use:b={p}
	use:c={p}
	use:d={p}
	use:e={p}
	use:f={p}
	use:g={p}
	use:h={ppppppppppp}
></div>

<!-- Ternary expression -->

<!-- 100 chars attribute line - ternary inline -->
<div
	use:action={cond ? optAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA : optBBBBBBBBBBBBBBBBBB}
></div>

<!-- 101 chars attribute line - ternary wraps -->
<div
	use:action={cond
		? optAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
		: optBBBBBBBBBBBBBBBBBBB}
></div>

<!-- Arrow function expression -->

<!-- Short arrow - stays inline -->
<div use:action={(node) => console.log(node)}></div>

<!-- Arrow with block body - wraps block -->
<div
	use:action={(node) => {
		console.log(node);
		console.log(node);
	}}
></div>

<!-- Function call argument wrapping -->

<!-- 100 chars attribute line - args inline -->
<div
	use:action={fn(aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaaaaa)}
></div>

<!-- 101 chars attribute line - args wrap -->
<div
	use:action={fn(
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaaaaaa,
	)}
></div>

<!-- NewExpression (similar to CallExpression) -->

<!-- 100 chars attribute line - args inline -->
<div
	use:action={new Ctor(aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaaaaaaaaa)}
></div>

<!-- 101 chars attribute line - args wrap -->
<div
	use:action={new Ctor(
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaaaaaaaaaa,
	)}
></div>

<!-- ArrayExpression -->

<!-- 100 chars attribute line - elements inline -->
<div
	use:action={[aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaa, aaaaaaaa]}
></div>

<!-- 101 chars attribute line - elements wrap -->
<div
	use:action={[
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaa,
		aaaaaaaaa,
	]}
></div>
