# Changelog

## 0.1.0 (2026-07-07)


### Features

* **core:** add cheapest-insertion construction with refill support ([705bbd7](https://github.com/green-ecolution/streamlet/commit/705bbd755e9d86e9b56bbc3c4a0ed7d4c81cac80))
* **core:** add cost matrix over problem nodes ([833b737](https://github.com/green-ecolution/streamlet/commit/833b737843d85ad53e97523b91f46b7d513555d0))
* **core:** add duration segment with time-warp and multi-trip support ([cc5ae70](https://github.com/green-ecolution/streamlet/commit/cc5ae7010238819d20cdec509c628e23b13cef3c))
* **core:** add inter-route operators and refill repositioning ([425297d](https://github.com/green-ecolution/streamlet/commit/425297d161c7618ff39c54238ad554aeddbd5e3b))
* **core:** add intra-route local search (relocate, swap, 2-opt) ([5355c2b](https://github.com/green-ecolution/streamlet/commit/5355c2bf9ae78b1794b249fe7acc2a0e939ad094))
* **core:** add load segment for O(1) capacity evaluation ([3077681](https://github.com/green-ecolution/streamlet/commit/30776812f4ddb41c0f85e3d2e3d32a774fb3e257))
* **core:** add router port for external routing engines ([2f25105](https://github.com/green-ecolution/streamlet/commit/2f251058bb2b1cd9908eb95ef3e8b2521ba5624e))
* **core:** add solve facade mapping plans to domain solutions ([d804eb9](https://github.com/green-ecolution/streamlet/commit/d804eb92be7a3c7c7bd5b1072d76880813c6b44a))
* **core:** add solver instance model and route evaluation ([02babb8](https://github.com/green-ecolution/streamlet/commit/02babb8c4e669ffb2ae15517eaa6c4004aa2d446))
* **core:** scaffold cargo workspace and VRP domain model ([60b6d0f](https://github.com/green-ecolution/streamlet/commit/60b6d0f9a619626cd92fad478aeaf6776fe58b44))
* init commit ([6d1ed89](https://github.com/green-ecolution/streamlet/commit/6d1ed896a6f1de6304cdde81751e554348b2444c))
* **pbf-patch:** add haversine and point-to-line geometry ([c04bd2d](https://github.com/green-ecolution/streamlet/commit/c04bd2ddd24045dd99aab0bb6083c8b5d81ad188))
* **pbf-patch:** add osm data model with car-accessible filter ([4e56a6d](https://github.com/green-ecolution/streamlet/commit/4e56a6d4d7f16cadda6fd2ffca22df224e5a9388))
* **pbf-patch:** add rtree way index with site matching ([e87bf5c](https://github.com/green-ecolution/streamlet/commit/e87bf5c8d3a90f8ef315c83e4522a8f420f22396))
* **pbf-patch:** derive changed ways for construction sites ([0554b55](https://github.com/green-ecolution/streamlet/commit/0554b55587929b0848fb3c33c53b7e73cddad460))
* **pbf-patch:** load car network from pbf ([ae500c1](https://github.com/green-ecolution/streamlet/commit/ae500c1a86b1f081adc8ea87842173218dcb268b))
* **pbf-patch:** parse construction sites from tbz verkehrsticker ([d0ded57](https://github.com/green-ecolution/streamlet/commit/d0ded57a74e5af31bd0c2cbc43a76aae80c7f82e))
* **pbf-patch:** scaffold crate with cli skeleton ([67fa299](https://github.com/green-ecolution/streamlet/commit/67fa299fba9f4fa06439fea4496e29ce75212740))
* **pbf-patch:** wire construction command ([76b87bc](https://github.com/green-ecolution/streamlet/commit/76b87bc0b9442489d4ed5a039d737147b5b82a2a))
* **pbf-patch:** write osc changesets ([c2d6cf2](https://github.com/green-ecolution/streamlet/commit/c2d6cf2da94a179e02c09b9f22b6d973e378d389))
* **server:** add application startup and main entrypoint ([e371d71](https://github.com/green-ecolution/streamlet/commit/e371d713b5820c90a1ee0961afb8541a8d45ef95))
* **server:** add env-based settings ([a1ab14f](https://github.com/green-ecolution/streamlet/commit/a1ab14f7093694f01f579b66f648051b8e3f3e86))
* **server:** add http layer with solve endpoint and error mapping ([6f348ea](https://github.com/green-ecolution/streamlet/commit/6f348ea81af36a45219b6f216cd7c5a0b8850bee))
* **server:** add solve service orchestrating matrix, solver, and geometry ([c7a226f](https://github.com/green-ecolution/streamlet/commit/c7a226f0504500445ed4ddd221403679aef4379e))
* **server:** add valhalla http client implementing the router port ([abb3b89](https://github.com/green-ecolution/streamlet/commit/abb3b8984e479faa43364f308c47812efa48ddf1))
* **server:** drain in-flight requests on shutdown signal ([cb2d9d7](https://github.com/green-ecolution/streamlet/commit/cb2d9d7a9d5119cc6e59cea4398555549fe4e7a8))


### Bug Fixes

* **core:** enforce shift end on depot return, cover refill evaluation ([9adf87e](https://github.com/green-ecolution/streamlet/commit/9adf87ec66d9ac2532535b177e1957393d008c96))
* **core:** make search iterations full passes, pin refill drop ([f430094](https://github.com/green-ecolution/streamlet/commit/f4300942b1d13c77b97248ffc93eb7888354e5ed))
* **core:** model reload as finalize, drop broken reload marker ([98f3b8b](https://github.com/green-ecolution/streamlet/commit/98f3b8b29e54cf172cd2849d3ccedd49f6364d83))
* **core:** validate cost matrix on deserialization ([309b153](https://github.com/green-ecolution/streamlet/commit/309b1533e8c47fdb2aef142925c444e849ca095e))
* **server:** guard matrix alignment, cover valhalla error paths ([9d16fd9](https://github.com/green-ecolution/streamlet/commit/9d16fd9e660bbff30b1ea52db305511f1c47a1f7))


### Miscellaneous Chores

* force initial release version ([78b3e76](https://github.com/green-ecolution/streamlet/commit/78b3e76b2ea6eaccb0562cb24790f95403b8d936))
