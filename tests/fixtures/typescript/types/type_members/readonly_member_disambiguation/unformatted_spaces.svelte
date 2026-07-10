<script lang="ts">
	interface I {
		// `readonly` modifier + string / number literal key
		readonly    'a-b'   :   A ;
		readonly   1:B;
		// `readonly` modifier + identifier key
		readonly c :  C;
		// `readonly` as the member name (not a modifier): property, optional
		// property, method, and generic method all named `readonly`
		readonly : D;
		readonly   ?:   E;
		readonly (  ) : void;
		readonly < T >( v :  T ) : void;
	}
</script>
