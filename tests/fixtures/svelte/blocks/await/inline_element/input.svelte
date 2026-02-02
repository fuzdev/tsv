<!-- Await block inside inline element - should stay inline when it fits -->
<a>{#await promise}text{/await}</a>
<span>{#await promise}text{:then val}{val}{/await}</span>
