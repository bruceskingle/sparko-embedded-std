# sparko-embedded-std
Hardware agnostic code for embedded systems development
## Introduction
The idea of Sparko Embedded is to provide a platform for embedded applications for hardware such as ESP32 SoC boards. Sparko Embedded Std is a version of this platform which includes the standard Rust library which means that the heap and standard collections like ```Vec``` are all available for use. This crate contains code for that platform which is hardware agnostic.

The platform makes use of the builder pattern as a way of trying to get the best trade off between the cost and benefits of the standard library. Most of the platform types have a builder containing collections which allow items to be added, but the ```build()``` method will usually call ```.shrink_to_fit()``` on those collections which are then treated as immutable from that point onwards.

The conventions for builders are that they
- are constructed by calling the ```builder()``` associated function on the class being constructed, e.g. ```TaskManager::builder()```
- provide chainable methods with names starting ```with_``` e.g. 
```
    TaskManager::builder()
        .with_task(task1)?
        .with_task(task2)?
        .build();
```
- provide callable methods with names starting ```add_``` e.g.
```
    let mut builder = TaskManager::builder()
        .with_task(task1)?;
    
    builder.add_task(task2)?;
    
    builder.build();
```
- have methods which either return their result directly (if there are no failure scenarios) or return an ```anyhow::Result<>``` of their result.

The following implementations are available which use this crate:
- sparko-esp-std for ESP32 SoC based boards