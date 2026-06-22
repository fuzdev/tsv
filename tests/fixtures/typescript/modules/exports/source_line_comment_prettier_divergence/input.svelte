<script lang="ts">
	// empty re-export: line comment between from and source
	export {} from // c
		'./a';

	// export-all: line comment between from and source
	export * from // c
		'./b';

	// named specifiers: line comment between from and source
	export { a } from // c
		'./c';
</script>
