class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.running = false
        this.proc = null
        this.queue = []

        this.dead_procs = []
        this.current_ref = null
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.proc.write(cmd.input)
		} else if (cmd.exec) {
            let invalid = await this.validate_exec(cmd.exec)
            if (invalid) {
                return invalid
            }

            if (this.running) {
                if (!cmd.exec.blocking) {
                    return api.Command.create({error: "Already running"})
                }

                this.queue.push(cmd)
                return
            }

            this.current_ref = cmd.ref
            await this.start_proc(cmd.exec)
        } else {
            console.debug("Unknown exec msg", cmd)
        }
	}

    async process_dead(proc_id, exit_code) {
        if (this.dead_procs.includes(proc_id) && proc_id !== -1) {return}
        this.dead_procs.push(proc_id)

        if (exit_code !== 0) {
            await this.send(api.Command.create({error: `exit status ${exit_code}`}), 0)
        }
        
        if (proc_id !== -1) {
            try {
                await this.proc.destroy()
            } catch (_) {}
        }

        await Deno.core.ops.op_sleep(100);
        await this.send(api.Command.create({state: api.State.Stopped}), 0)
        await this.send(api.Command.create({ok: {}, ref: this.current_ref}), 0)
        
        this.current_ref = null
        if (this.queue.length === 0) {
            return
        }
        
        let cmd = this.queue.shift();
        this.current_ref = cmd.ref
        await this.start_proc(cmd.exec)
    }

    async validate_exec(cmd) {
        if (cmd.args.length === 0) {
            return api.Command.create({error: "Missing command"})
        }
        return false
    }
    
    async resource_usage(cmd) {
        const is_cpu_req = cmd === "date '+%s%N' && cat /sys/fs/cgroup/cpu/cpuacct.usage /sys/fs/cgroup/cpu/cpu.cfs_quota_us /sys/fs/cgroup/cpu/cpu.cfs_period_us /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.soft_limit_in_bytes /sys/fs/cgroup/memory/memory.limit_in_bytes &&grep '^\\(total_rss\\|total_cache\\) ' /sys/fs/cgroup/memory/memory.stat";
        const is_storage_req = cmd === "cat /repl/stats/subvolume_usage_bytes /repl/stats/subvolume_total_bytes";
        // if (is_cpu_req) {
        //     return "date '+%s%N' && echo 100000 && echo 200000 && cat /sys/fs/cgroup/cpu/cpu.cfs_period_us /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.soft_limit_in_bytes /sys/fs/cgroup/memory/memory.limit_in_bytes &&grep '^\\(total_rss\\|total_cache\\) ' /sys/fs/cgroup/memory/memory.stat";
        // } else 
        if (is_cpu_req || is_storage_req) {
            await this.send(api.Command.create({state: api.State.Running}), 0)
            let output = "";

            if (is_storage_req) {
                const disk = await Deno.core.ops.op_disk_info();
                output = `${disk.free}\n${disk.total}\n`
            }
            if (is_cpu_req) {
                const cpuTime = await Deno.core.ops.op_cpu_info();
                const memory = await Deno.core.ops.op_memory_info();
                const memoryUsage = memory.total - memory.free;
                const totalMemory = memory.total
                output = `${new Number(Date.now()) * 1000000}\n${cpuTime}\n200000\n100000\n${memoryUsage}\n${totalMemory}\n${totalMemory}\ntotal_cache 0\ntotal_rss ${memoryUsage}`
            }

            await this.send(api.Command.create({output}), 0)
            
            await this.process_dead(-1, 0)

            return false
        } else {
            return cmd
        }
    }

    async start_proc(msg) {
        const cmd = msg.args[0];

        if (msg.args.length === 3 && msg.args[0] === "bash" && msg.args[1] === "-c") {
            let arg = msg.args[2]

            let new_cmd = await this.resource_usage(arg)
            if (new_cmd) {
                msg.args[2] = new_cmd
            } else {
                return
            }
        }
        const args = msg.args.slice(1)
        const env = msg.env ? msg.env : {}

        
        await this.send(api.Command.create({state: api.State.Running}), 0)
        this.proc = new Process(this.id, cmd, args, env)
        
        await this.proc.init(this.clients)

    }

    async attach(session) {
        if (this.proc) await this.proc.add_session(session)
    }

    async detach(session) {
        if (this.proc) await this.proc.remove_session(session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
