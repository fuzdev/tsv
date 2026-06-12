<script lang="ts">
	type A=string;

	type B=string|number;

	type C=A&B;

	type D={
		prop:string;
	};

	type D2={prop:string};

	type E=(x:number)=>string;

	type F=[string,number];

	type G=string[];

	export type H=string|number;
</script>
