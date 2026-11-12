<!-- "Agent working" indicator: a solid dot in the project colour with an expanding ring around it.
     Expects --project to be set by an ancestor. -->
<span class="relative flex size-2.5 shrink-0" title="Agent working">
  <span class="absolute inline-flex size-full rounded-full bg-(--project) opacity-75 motion-safe:animate-ping"
  ></span>
  <span class="relative inline-flex size-2.5 rounded-full bg-(--project) ring-2 ring-(--project)/30"></span>
</span>
