<script lang="ts">
	// block comment between ) and body — total exactly 100 chars, stays inline
	for(let iiii=0;iiii<someBoundaryNameHereXYZABCDEFGHIJKLMNOPQ;iiii++)/* b */ doStuff(iiii);

	// block comment between ) and body — overflow drops comment + body to their own line
	for(let iiii=0;iiii<someLongBoundaryNameThatGoesHereXYZABCDEFGHIJKLMNO;iiii++)/* b */ doStuff(iiii);

	// for-of overflow — comment + body drop together
	for(const item of someLongIterableNameThatGoesHereXYZABCDEFGHIJKLMNOPQRSTUV)/* b */ doStuff(item);

	// for-in overflow — comment + body drop together
	for(const key in someLongObjectNameThatGoesHereXYZABCDEFGHIJKLMNOPQRSTUVWXYZ)/* b */ doStuff(key);
</script>
