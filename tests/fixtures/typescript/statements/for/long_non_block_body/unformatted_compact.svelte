<script lang="ts">
	// total exactly 100 chars — stays inline
	for(let iiii=0;iiii<someLongBoundaryNameThatGoesHereXYZABCDE;iiii++)doSomethingHere(iiii);

	// total 101 chars — body moves to its own indented line, header stays flat
	for(let iiii=0;iiii<someLongBoundaryNameThatGoesHereXYZABCDEF;iiii++)doSomethingHere(iiii);

	// for-of — total exactly 100 chars, stays inline
	for(const item of someLongIterableNameThatGoesHereXYZABCDEFGHIJKL)doSomethingWithThisItem(item);

	// for-of — total 101 chars, body moves to its own indented line
	for(const item of someLongIterableNameThatGoesHereXYZABCDEFGHIJKL)doSomethingWithThisItemX(item);

	// for-in — total exactly 100 chars, stays inline
	for(const key in someLongObjectNameThatGoesHereXYZABCDEFGHIJKLMNOP)doSomethingWithThisKeyy(key);

	// for-in — total 101 chars, body moves to its own indented line
	for(const key in someLongObjectNameThatGoesHereXYZABCDEFGHIJKLMNOP)doSomethingWithThisKeyyy(key);
</script>
