<script lang="ts">
	// blank after a head comment, before the first attribute
	import a from './a' with {
		// c1

		type: 'json'
	};

	// blank between attributes (an author blank forces the clause open)
	import b from './b' with {
		type: 'json',

		foo: 'bar'
	};

	// blank before an own-line comment following an attribute
	import c from './c' with {
		type: 'json',

		// c2
		foo: 'bar'
	};

	// blank after an own-line comment following an attribute
	import d from './d' with {
		type: 'json',
		// c3

		foo: 'bar'
	};
</script>
