@mutex.synchronize { @executor ||= ThreadedExecutor.new(max_size: config.executor_pool_size, name: "streamer", wait: true) }
