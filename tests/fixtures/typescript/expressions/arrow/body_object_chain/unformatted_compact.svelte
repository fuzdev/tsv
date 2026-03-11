<script lang="ts">
	// Arrow body: object with post-expression operators needing parens

	// Computed member access
	const a = (x: string) => ({a:"x"})[x];

	// Dot member access
	const b = () => ({a:1,b:2}).toString();

	// Multiline with computed access
	const c = (x: string) => ({a:"x",b:"y",c:"z"})[x];

	// 100 boundary — object + as stays flat (100 chars)
	const d = (x = 'a',  y = 'b'):  T =>({a:  x as A,  b:  x,  c:  'value________________________',  d:  `/tmp/${x}`,  e:  {a:  'x',  b:  y}}) as T;

	// 101 boundary — object + as wraps (101 chars)
	const e = (x = 'a',  y = 'b'):  T =>({a:  x as A,
		b:  x,
		c:  'value_________________________',
		d:  `/tmp/${x}`,
		e:  {a:  'x',  b:  y},
		}) as T;

	// Control: plain arrow body object (no chain)
	const f = () => ({a:1,b:2});
</script>
