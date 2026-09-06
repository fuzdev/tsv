<script lang="ts">
	// An indentable multi-line block comment glued to the value hangs the value under the
	// `=` ahead of every layout the binding itself would choose: a typed binding over a
	// type-assertion call, a complex type annotation, a complex destructuring pattern, a
	// breakable binding over an arrow (pattern and typed), and a curried arrow chain.
	const a1:A=/**
	 * c
	 */fn(x,y) as A;
	const a2:Map<string,Array<number>>=/**
	 * c
	 */fn(x);
	const {b1=1,b2=2,b3}=/**
	 * c
	 */obj;
	const {c1,c2}=/**
	 * c
	 */(x)=>x;
	const d:(x:number)=>void=/**
	 * c
	 */(x)=>{};
	const e=/**
	 * c
	 */(p)=>(q)=>p;

	// A preserved multi-line block comment glued to the value keeps the value on the `=`
	// line, under the same bindings.
	const f1:A=/* line1
	line2 */fn(x,y) as A;
	const f2:Map<string,Array<number>>=/* line1
	line2 */fn(x);
	const h:(x:number)=>void=/* line1
	line2 */(x)=>{};
	const i=/* line1
	line2 */(p)=>(q)=>p;
</script>
