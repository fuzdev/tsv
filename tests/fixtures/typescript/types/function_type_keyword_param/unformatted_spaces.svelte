<script lang="ts">
	// Contextual type keywords (number, string, …) used as parameter names in a
	// function type — distinct from the parenthesized type `(number)`.
	type F1 = ( number ) => U;
	type F2 = ( string , boolean ) => U;
	type F3 = ( number : T ) => U;
	type F4 = ( object ? ) => U;
	type F5 = ( undefined ) => U;
</script>
