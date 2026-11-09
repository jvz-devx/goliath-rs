"""Owns unchanged startup-call observations before any M3 fault is admitted."""
import json
from pathlib import Path
import struct
import time

import e11_support as support


def observe_clock_calls(module,folder):
    original=module.store.call
    calls=[]
    def call(address,message):
        operation=message[17]
        if operation not in {13,14}:return original(address,message)
        row=dict(operation=operation,request_id=str(int.from_bytes(message[1:17],'little')),
            sent_monotonic_ns=time.monotonic_ns(),sent_wall_ns=time.time_ns())
        if operation==14:row['fence']=list(struct.unpack_from('<QQQ',message,18))
        try:
            response=original(address,message)
            row['response_hex']=response.hex()
            return response
        except BaseException as error:
            row['error']=str(error)
            raise
        finally:
            row.update(received_monotonic_ns=time.monotonic_ns(),received_wall_ns=time.time_ns())
            calls.append(row)
            support.save(Path(folder)/'clock-calls.json',dict(calls=calls,retries_added=0,semantics_changed=False))
    module.store.call=call
    return calls
