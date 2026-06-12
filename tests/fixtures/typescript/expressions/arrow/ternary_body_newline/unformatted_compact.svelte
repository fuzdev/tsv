<script lang="ts">
	type A=string|number|Date;
	const fn=(v:A,f:string)=>'';

	/* long signature forces body to new line - ternary without parens */
	const fn1=(a:A|null|undefined,b='',c='',d=''):string=>!a?b:fn(a,'value');

	const fn2=(x:string|null|undefined,y:string,z:number):string=>x?fn(`/a/${x}`,y):fn('/a',y);

	/* async arrow */
	const fn3=async(a:string|null|undefined,b:string,c:number):Promise<string>=>a?await fn(a,b):await fn(b,b);

	/* nested ternary */
	const fn4=(a:string|null|undefined,b:string,c:string,d:string):string=>a?b:c?d:b;
</script>
