import {toBytesArrayU8, toBytesString, toBytesI32, toBytesStringList, toBytesU64} from "./bytesrepr";
import {U512} from "./bignum";
import {URef} from "./uref";
import {Key} from "./key";
import {Option} from "./option";

/**
 * CasperLabs types, i.e. types which can be stored and manipulated by smart contracts.
 *
 * Provides a description of the underlying data type of a [[CLValue]].
 */
export enum CLTypeTag {
    /** A boolean value */
    Bool = 0,
    /** A 32-bit signed integer */
    I32 = 1,
    /** A 64-bit signed integer */
    I64 = 2,
    /** An 8-bit unsigned integer (a byte) */
    U8 = 3,
    /** A 32-bit unsigned integer */
    U32 = 4,
    /** A 64-bit unsigned integer */
    U64 = 5,
    /** A 128-bit unsigned integer */
    U128 = 6,
    /** A 256-bit unsigned integer */
    U256 = 7,
    /** A 512-bit unsigned integer */
    U512 = 8,
    /** A unit type, i.e. type with no values (analogous to `void` in C and `()` in Rust) */
    Unit = 9,
    /** A string of characters */
    String = 10,
    /** A key in the global state - URef/hash/etc. */
    Key = 11,
    /** An Unforgeable Reference (URef) */
    Uref = 12,
    /** An [[Option]], i.e. a type that can contain a value or nothing at all */
    Option = 13,
    /** A list of values */
    List = 14,
    /** A fixed-length list of values */
    Fixed_list = 15,
    /**
     * A [[Result]], i.e. a type that can contain either a value representing success or one representing failure.
     */
    Result = 16,
    /** A key-value map. */
    Map = 17,
    /** A 1-value tuple. */
    Tuple1 = 18,
    /** A 2-value tuple, i.e. a pair of values. */
    Tuple2 = 19,
    /** A 3-value tuple. */
    Tuple3 = 20,
    /** A value of any type. */
    Any = 21,
}

/**
 * A CasperLabs value, i.e. a value which can be stored and manipulated by smart contracts.
 *
 * It holds the underlying data as a type-erased, serialized array of bytes and also holds the
 * [[CLType]] of the underlying data as a separate member.
 */
export class CLValue {
    bytes: u8[];
    tag: u8[];

    /**
     * Constructs a new `CLValue` with given underlying data and type tag.
     */
    constructor(bytes: u8[], tag: u8[]) {
        this.bytes = bytes;
        this.tag = tag;
    }

    /**
     * Creates a `CLValue` holding a string.
     */
    static fromString(s: String): CLValue {
        return new CLValue(toBytesString(s), [<u8>CLTypeTag.String]);
    }

    /**
     * Creates a `CLValue` holding an unsigned 512-bit integer.
     */
    static fromU512(value: U512): CLValue {
        return new CLValue(value.toBytes(), [<u8>CLTypeTag.U512]);
    }

    /**
     * Creates a `CLValue` holding a signed 32-bit integer.
     */
    static fromI32(value: i32): CLValue {
        return new CLValue(toBytesI32(value), [<u8>CLTypeTag.I32]);
    }

    /**
     * Creates a `CLValue` holding an unsigned 64-bit integer.
     */
    static fromU64(value: u64): CLValue {
        return new CLValue(toBytesU64(value), [<u8>CLTypeTag.U64]);
    }

    /**
     * Creates a `CLValue` holding a [[Key]].
     */
    static fromKey(key: Key): CLValue{
        return new CLValue(key.toBytes(), [<u8>CLTypeTag.Key]);
    }

    /**
     * Creates a `CLValue` holding a [[URef]].
     */
    static fromURef(uref: URef): CLValue {
        return new CLValue(uref.toBytes(), [<u8>CLTypeTag.Uref]);
    }

    /**
     * Creates a `CLValue` holding a list of strings.
     */
    static fromStringList(values: String[]): CLValue {
        return new CLValue(toBytesStringList(values), [
            <u8>CLTypeTag.List,
            <u8>CLTypeTag.String,
        ]);
    }

    /**
     * Creates a `CLValue` holding an [[Option]].
     */
    static fromOption(value: Option, nestedT: CLTypeTag): CLValue {
        return new CLValue(value.toBytes(), [
            <u8>CLTypeTag.Option,
            <u8>nestedT,
        ]);
    }

    /**
     * Serializes a `CLValue` into an array of bytes.
     */
    toBytes(): u8[] {
        let data = toBytesArrayU8(this.bytes);
        for (let i = 0; i < this.tag.length; i++) {
            data.push(this.tag[i]);
        }
        return data;
    }
}
