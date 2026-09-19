package script

import (
	"errors"
	"math/rand/v2"

	"github.com/arnodel/golua/lib/oslib"
	lua "github.com/arnodel/golua/runtime"
)

// Random state belongs to one execution, never the host or another script.
func (s *luaSandbox) loadAgentTimeAndRandom(mathTable *lua.Table) {
	generator := rand.New(rand.NewPCG(rand.Uint64(), rand.Uint64()))
	random := s.runtime.SetEnvGoFunc(mathTable, "random", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		if call.NArgs() == 0 {
			return call.PushingNext1(thread.Runtime, lua.FloatValue(generator.Float64())), nil
		}
		lower, upper := int64(1), int64(0)
		first, err := call.IntArg(0)
		if err != nil {
			return nil, err
		}
		upper = first
		if call.NArgs() > 1 {
			lower = first
			upper, err = call.IntArg(1)
			if err != nil {
				return nil, err
			}
		} else if upper == 0 {
			return call.PushingNext1(thread.Runtime, lua.IntValue(int64(generator.Uint64()))), nil
		}
		if lower > upper {
			return nil, errors.New("random interval is empty")
		}
		span := uint64(upper) - uint64(lower) + 1
		var value uint64
		if span == 0 {
			value = generator.Uint64()
		} else {
			value = uint64(lower) + generator.Uint64N(span)
		}
		return call.PushingNext1(thread.Runtime, lua.IntValue(int64(value))), nil
	}, 2, false)
	seed := s.runtime.SetEnvGoFunc(mathTable, "randomseed", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		first, second := int64(rand.Uint64()), int64(rand.Uint64())
		var err error
		if call.NArgs() > 0 {
			first, err = call.IntArg(0)
			if err != nil {
				return nil, err
			}
			second = 0
		}
		if call.NArgs() > 1 {
			second, err = call.IntArg(1)
			if err != nil {
				return nil, err
			}
		}
		generator = rand.New(rand.NewPCG(uint64(first), uint64(second)))
		return call.PushingNext(thread.Runtime, lua.IntValue(first), lua.IntValue(second)), nil
	}, 2, false)

	// Copy only the clock helpers; never expose the complete OS library.
	library, _ := oslib.LibLoader.Load(s.runtime)
	source, _ := library.TryTable()
	clock := s.newTable()
	for _, name := range []string{"clock", "difftime"} {
		s.runtime.SetEnv(clock, name, lua.RawGet(source, lua.StringValue(name)))
	}
	originalDate := lua.RawGet(source, lua.StringValue("date"))
	dateFunction := s.runtime.SetEnvGoFunc(clock, "date", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		var arguments []lua.Value
		if call.NArgs() > 0 {
			format, err := call.StringArg(0)
			if err != nil || len(format) > luaTextLimit {
				return nil, errors.New("date format is invalid or too large")
			}
			arguments = append(arguments, call.Arg(0))
		}
		if call.NArgs() > 1 {
			arguments = append(arguments, call.Arg(1))
		}
		value, err := lua.Call1(thread, originalDate, arguments...)
		if err != nil {
			return nil, err
		}
		if text, ok := value.TryString(); ok {
			thread.RequireBytes(len(text))
		} else {
			thread.RequireBytes(luaHostTableCharge)
		}
		return call.PushingNext1(thread.Runtime, value), nil
	}, 2, false)
	originalTime := lua.RawGet(source, lua.StringValue("time"))
	timeFunction := s.runtime.SetEnvGoFunc(clock, "time", func(thread *lua.Thread, call *lua.GoCont) (lua.Cont, error) {
		var arguments []lua.Value
		if call.NArgs() > 0 {
			date, err := call.TableArg(0)
			if err != nil {
				return nil, err
			}
			// Lua normalizes date fields in place. Keep the supplied table unchanged.
			copy := s.newTable()
			for _, name := range []string{"year", "month", "day", "hour", "min", "sec"} {
				value, err := lua.Index(thread, lua.TableValue(date), lua.StringValue(name))
				if err != nil {
					return nil, err
				}
				s.runtime.SetEnv(copy, name, value)
			}
			arguments = []lua.Value{lua.TableValue(copy)}
		}
		value, err := lua.Call1(thread, originalTime, arguments...)
		if err != nil {
			return nil, err
		}
		return call.PushingNext1(thread.Runtime, value), nil
	}, 1, false)
	lua.SolemnlyDeclareCompliance(lua.ComplyMemSafe|lua.ComplyCpuSafe|lua.ComplyTimeSafe|lua.ComplyIoSafe, random, seed, timeFunction, dateFunction)
	s.runtime.SetEnv(s.runtime.GlobalEnv(), "os", lua.TableValue(clock))
}
