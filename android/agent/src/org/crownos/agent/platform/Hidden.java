package org.crownos.agent.platform;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Reflection over hidden framework APIs. Every failure becomes a {@link HiddenApiException} that
 * names the member, so a changed image is reported instead of crashing the agent.
 */
public final class Hidden {
    public static final Class<?>[] NO_TYPES = new Class<?>[0];

    private static final Map<String, Method> METHODS = new ConcurrentHashMap<>();
    private static final Map<String, Field> FIELDS = new ConcurrentHashMap<>();

    private Hidden() {
    }

    public static Class<?> type(String className) {
        try {
            return Class.forName(className);
        } catch (ClassNotFoundException | LinkageError e) {
            throw new HiddenApiException("class not found: " + className, e);
        }
    }

    public static Class<?>[] types(Object... classesOrNames) {
        Class<?>[] types = new Class<?>[classesOrNames.length];
        for (int index = 0; index < types.length; index++) {
            Object entry = classesOrNames[index];
            types[index] = entry instanceof Class ? (Class<?>) entry : type((String) entry);
        }
        return types;
    }

    public static Object call(Object receiver, String methodName, Class<?>[] parameterTypes, Object... arguments) {
        if (receiver == null) {
            throw new HiddenApiException("cannot call " + methodName + " on null", null);
        }
        return invoke(receiver.getClass(), receiver, methodName, parameterTypes, arguments);
    }

    public static Object callStatic(Class<?> owner, String methodName, Class<?>[] parameterTypes,
            Object... arguments) {
        return invoke(owner, null, methodName, parameterTypes, arguments);
    }

    public static Object construct(Class<?> owner, Class<?>[] parameterTypes, Object... arguments) {
        try {
            Constructor<?> constructor = owner.getDeclaredConstructor(parameterTypes);
            constructor.setAccessible(true);
            return constructor.newInstance(arguments);
        } catch (ReflectiveOperationException | LinkageError e) {
            throw new HiddenApiException("cannot construct " + owner.getName(), unwrap(e));
        }
    }

    public static Object field(Object receiver, String fieldName) {
        try {
            return findField(receiver.getClass(), fieldName).get(receiver);
        } catch (IllegalAccessException e) {
            throw new HiddenApiException("cannot read field " + fieldName, e);
        }
    }

    public static int intField(Object receiver, String fieldName) {
        return (Integer) field(receiver, fieldName);
    }

    public static boolean booleanField(Object receiver, String fieldName) {
        return (Boolean) field(receiver, fieldName);
    }

    private static Object invoke(Class<?> owner, Object receiver, String methodName, Class<?>[] parameterTypes,
            Object[] arguments) {
        try {
            return findMethod(owner, methodName, parameterTypes).invoke(receiver, arguments);
        } catch (ReflectiveOperationException | LinkageError e) {
            throw new HiddenApiException(owner.getName() + "." + methodName + " failed", unwrap(e));
        }
    }

    private static Method findMethod(Class<?> owner, String methodName, Class<?>[] parameterTypes)
            throws NoSuchMethodException {
        String key = owner.getName() + '#' + methodName + Arrays.toString(parameterTypes);
        Method method = METHODS.get(key);
        if (method == null) {
            method = owner.getMethod(methodName, parameterTypes);
            method.setAccessible(true);
            METHODS.put(key, method);
        }
        return method;
    }

    private static Field findField(Class<?> type, String fieldName) {
        String key = type.getName() + '#' + fieldName;
        Field cached = FIELDS.get(key);
        if (cached != null) {
            return cached;
        }
        for (Class<?> current = type; current != null; current = current.getSuperclass()) {
            for (Field candidate : current.getDeclaredFields()) {
                if (candidate.getName().equals(fieldName)) {
                    candidate.setAccessible(true);
                    FIELDS.put(key, candidate);
                    return candidate;
                }
            }
        }
        throw new HiddenApiException("field not found: " + type.getName() + "." + fieldName, null);
    }

    private static Throwable unwrap(Throwable e) {
        return e instanceof InvocationTargetException ? e.getCause() : e;
    }
}
