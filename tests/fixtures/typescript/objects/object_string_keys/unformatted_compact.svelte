<script>
	const needsQuotes = {'kebab-case':1,'with space':2,'123numeric':3};
	const noQuotesNeeded = {validId:1,_underscore:2,$dollar:3};
	const reserved = {class:1,function:2,return:3};
	const empty = {'':1};
	const mixed = {normal:1,'hyphen-key':2,another:3};
	const nested = {outer:{'inner-key':{deep:1}}};
</script>
