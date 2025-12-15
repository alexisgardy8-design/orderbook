import msgpack
import time

def float_to_wire(x: float) -> str:
    rounded = f"{x:.8f}"
    if abs(float(rounded) - x) >= 1e-12:
        raise ValueError("float_to_wire causes rounding", x)
    if rounded == "-0":
        rounded = "0"
    # Simple normalization: remove trailing zeros and dot
    s = rounded.rstrip('0').rstrip('.')
    if s == "": s = "0"
    return s

def get_msgpack_hex():
    asset_index = 0
    is_buy = True
    px = 13.291
    sz = 0.01
    
    order_wire = {
        "a": asset_index,
        "b": is_buy,
        "p": float_to_wire(px),
        "s": float_to_wire(sz),
        "r": False,
        "t": {"limit": {"tif": "Gtc"}}
    }
    
    action = {
        "type": "order",
        "orders": [order_wire],
        "grouping": "na"
    }
    
    packed = msgpack.packb(action)
    print(f"MsgPack Hex: {packed.hex()}")

if __name__ == "__main__":
    get_msgpack_hex()
