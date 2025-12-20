<script lang="ts">
	// Arrow functions with string literal return types
	// Tests lookahead scanner handling of string literals in type annotations

	// Single string literal return type
	const single = (x:string):'a' => 'a';

	// Union of string literals
	const union = (x:string):'a'|'b'|'c' => 'a';

	// With typed parameter
	const typed = (a:T):'a'|'b'|'c' => 'a';

	// Empty params with string literal return
	const empty = ():'a'|'b' => 'a';

	// Generic with string literal return
	const generic = <T,>(x:T):'a'|'b' => 'a';

	// Async with string literal return
	const asyncFn = async (x:number):Promise<'a'|'b'> => 'a';

	// Multiple parameters
	const multiple = (a:string,b:number):'a'|'b' => 'a';
</script>
